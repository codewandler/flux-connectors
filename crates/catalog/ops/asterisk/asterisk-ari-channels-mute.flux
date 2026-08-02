op asterisk-ari-channels-mute(channelId: String, direction: String) -> Any
  description "Mute a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/mute")
  sep = "?"
  when direction
    url = fmt("{url}{sep}direction={direction}")
  response = http.request(method: "POST", url)
  return response
