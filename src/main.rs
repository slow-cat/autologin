use anyhow::Result;
use futures_util::StreamExt;
use reqwest::{Client, StatusCode};
use std::{fs::read_to_string, time::Duration};
use tokio::time::{self, Interval};
use zbus::{Connection, Proxy, fdo::ObjectManagerProxy, zvariant::OwnedObjectPath};

const PROBE: &str = "https://connectivitycheck.gstatic.com/generate_204";
const AUTH: &str = "https://login.shinshu-u.ac.jp/cgi-bin/Login.cgi";
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::var("CREDENTIALS_DIRECTORY")
        .inspect_err(|e| eprintln!("CREDENTIALS_DIRECTORY: {e}"))?;
    let uid = read_to_string(format!("{dir}/uid")).inspect_err(|e| eprintln!("UID FILE: {e}"))?;
    let pwd = read_to_string(format!("{dir}/pwd")).inspect_err(|e| eprintln!("PWD FILE: {e}"))?;
    let bus = Connection::system().await?;
    let http = reqwest::Client::builder()
        .use_native_tls()
        .timeout(Duration::from_secs(5))
        .connect_timeout(Duration::from_secs(1))
        .build()?;

    let networkd = Proxy::new(
        &bus,
        "org.freedesktop.network1",
        "/org/freedesktop/network1",
        "org.freedesktop.network1.Manager",
    )
    .await?;

    let mut changes = networkd
        .receive_property_changed::<String>("AddressState")
        .await;

    let mut probe_timer: Option<Interval> = None;
    let mut offline_timer: Option<Interval> = None;
    loop {
        tokio::select! {
            Some(change) = changes.next() => {
                let routable = change.get().await? == "routable";
                if routable {
                    println!("changed: routable");
                    probe_timer=Some(time::interval(Duration::from_secs(180)));
                }else{
                    println!("changed: not routable");
                    probe_timer=None;
                }
            }
            _=async{probe_timer.as_mut().unwrap().tick().await},
                if probe_timer.is_some()=>{
                    if let Err(e) = check_validation(&bus, &http, &mut offline_timer).await{
                        eprintln!("validation error: {e}")
                    };
                }
            _=async{offline_timer.as_mut().unwrap().tick().await},
                if offline_timer.is_some()=>{
                    if let Err(e) = login(&bus, &http, &mut offline_timer,&uid,&pwd).await{
                        eprintln!("login error: {e:#}")
                    };
                }
        }
    }
}
async fn get_ssid(bus: &Connection) -> Result<String> {
    let objects = ObjectManagerProxy::builder(bus)
        .destination("net.connman.iwd")?
        .path("/")?
        .build()
        .await?
        .get_managed_objects()
        .await?;

    if let Some(path) = objects.iter().find_map(|(path, ifaces)| {
        ifaces
            .contains_key("net.connman.iwd.Station")
            .then(|| path.clone())
    }) {
        let station = Proxy::new(bus, "net.connman.iwd", path, "net.connman.iwd.Station").await?;

        if let Ok::<OwnedObjectPath, _>(net) = station.get_property("ConnectedNetwork").await {
            let network =
                Proxy::new(&bus, "net.connman.iwd", net, "net.connman.iwd.Network").await?;

            let ssid: String = network.get_property("Name").await?;
            return Ok(ssid);
        }
    }
    Err(anyhow::anyhow!("no connected WiFi network"))
}

async fn check_validation(
    bus: &Connection,
    http: &Client,
    offline_timer: &mut Option<Interval>,
) -> Result<(), Box<dyn std::error::Error>> {
    let online = http
        .get(PROBE)
        .send()
        .await
        .is_ok_and(|r| r.status() == StatusCode::NO_CONTENT);

    println!("{}", if online { "online" } else { "offline" });
    let ssid = get_ssid(bus).await?;
    if !online
        && matches!(
            ssid.as_str(),
            "engineering-wifi" | "cs115" | "cs416" | "cs215" | "cs316"
        )
    {
        if offline_timer.is_none() {
            *offline_timer = Some(time::interval(Duration::from_secs(10)));
        }
    } else {
        *offline_timer = None;
    }
    Ok(())
}
async fn login(
    bus: &Connection,
    http: &Client,
    offline_timer: &mut Option<Interval>,
    uid: &String,
    pwd: &String,
) -> Result<()> {
    let ssid = get_ssid(bus).await?;
    println!("SSID: {ssid}");
    if matches!(
        ssid.as_str(),
        "engineering-wifi" | "cs115" | "cs416" | "cs215" | "cs316"
    ) {
        match http
            .post(AUTH)
            .form(&[("uid", uid.as_str()), ("pwd", pwd.as_str())])
            .send()
            .await
        {
            Ok(r) => println!("login :{}", r.status()),
            Err(e) => return Err(e.into()),
        };
        println!("Send to lgi");
        let online = http
            .get(PROBE)
            .send()
            .await
            .is_ok_and(|r| r.status() == StatusCode::NO_CONTENT);
        println!("{}", if online { "online" } else { "offline" });
        if online {
            *offline_timer = None;
        }
    }

    Ok(())
}
