op asterisk-ari-channels-get-channel-var(channelId: String, variable: String) -> Any
  description "Get the value of a channel variable or function."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variable?variable={variable}")
  response = http.request(method: "GET", url)
  return response
