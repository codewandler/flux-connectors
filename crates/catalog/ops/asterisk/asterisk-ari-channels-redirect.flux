op asterisk-ari-channels-redirect(channelId: String, endpoint: String) -> Any
  description "Redirect the channel to a different location."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/redirect")
  response = http.request(method: "POST", query: { endpoint }, url)
  return response
