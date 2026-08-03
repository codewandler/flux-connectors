op asterisk-ari-channels-unmute(channelId: String, direction: String) -> Any
  description "Unmute a channel."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/mute")
  response = http.request(method: "DELETE", query: { direction }, url)
  return response
