op asterisk-ari-channels-progress(channelId: String) -> Any
  description "Indicate progress on a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/progress")
  response = http.request(method: "POST", url)
  return response
