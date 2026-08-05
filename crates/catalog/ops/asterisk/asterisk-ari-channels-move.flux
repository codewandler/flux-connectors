op asterisk-ari-channels-move(channelId: String, app: String, appArgs: String) -> Any
  description "Move the channel from one Stasis application to another."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/move")
  response = http.request(method: "POST", query: { app, appArgs }, url)
  return response
