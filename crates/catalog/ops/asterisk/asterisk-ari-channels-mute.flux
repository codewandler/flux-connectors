op asterisk-ari-channels-mute(channelId: String, direction: String) -> Any
  description "Mute a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/mute")
  response = http.request(method: "POST", query: { direction }, url)
  return response
