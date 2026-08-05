op asterisk-ari-channels-hold(channelId: String) -> Any
  description "Hold a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/hold")
  response = http.request(method: "POST", url)
  return response
