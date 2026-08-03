op asterisk-ari-channels-start-moh(channelId: String, mohClass: String) -> Any
  description "Play music on hold to a channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/moh")
  response = http.request(method: "POST", query: { mohClass }, url)
  return response
