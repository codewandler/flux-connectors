op asterisk-ari-channels-move(channelId: String, app: String, appArgs: String) -> Any
  description "Move the channel from one Stasis application to another."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/move?app={app}")
  sep = "&"
  when appArgs
    url = fmt("{url}{sep}appArgs={appArgs}")
  response = http.request(method: "POST", url)
  return response
