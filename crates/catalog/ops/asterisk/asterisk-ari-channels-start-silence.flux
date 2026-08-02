op asterisk-ari-channels-start-silence(channelId: String) -> Any
  description "Play silence to a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/silence")
  response = http.request(method: "POST", url)
  return response
