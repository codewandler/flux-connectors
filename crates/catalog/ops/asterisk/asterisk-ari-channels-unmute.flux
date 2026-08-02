op asterisk-ari-channels-unmute(channelId: String, direction: String) -> Any
  description "Unmute a channel."
  risk "destructive"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/mute")
  sep = "?"
  when direction
    url = fmt("{url}{sep}direction={direction}")
  response = http.request(method: "DELETE", url)
  return response
