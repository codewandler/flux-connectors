op asterisk-ari-channels-ring(channelId: String) -> Any
  description "Indicate ringing to a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/ring")
  response = http.request(method: "POST", url)
  return response
