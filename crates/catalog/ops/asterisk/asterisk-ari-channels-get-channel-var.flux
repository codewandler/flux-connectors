op asterisk-ari-channels-get-channel-var(channelId: String, variable: String) -> Any
  description "Get the value of a channel variable or function."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variable")
  response = http.request(method: "GET", query: { variable }, url)
  return response
