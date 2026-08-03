op asterisk-ari-channels-send-dtmf(channelId: String, dtmf: String, before: Number, between: Number, duration: Number, after: Number) -> Any
  description "Send provided DTMF to a given channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/dtmf")
  response = http.request(method: "POST", query: { after, before, between, dtmf, duration }, url)
  return response
