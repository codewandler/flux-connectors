op asterisk-ari-channels-get-channel-vars(channelId: String, variables: List<String>) -> Any
  description "Get the value of multiple channel variables or functions."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variables?variables={variables}")
  response = http.request(method: "GET", url)
  return response
