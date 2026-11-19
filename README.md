# 概要

systemd-networkdで信州大学WiFIに自動ログインするやつ
# 準備

テスト用にユーザーで動かす説明をする。
`/etc/secrets/shinshu-password`に`uid`と`pwd`ファイルを作成し
ログインに必要な情報を書き込んでおく。なおユーザーで読み込めるようにしとく。

```bash
#レポ直下にて
cargo install --path .
#ユーザー名を修正してautoconnect.serviceを.config/systemd/userに配置し
systemctl start autoconnect
```

なおWiFiSSIDは自分がよく使うものを現状ハードコーディングしているだけである 
