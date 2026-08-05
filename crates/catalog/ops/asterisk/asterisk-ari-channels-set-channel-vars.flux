op asterisk-ari-channels-set-channel-vars(channelId: String, variables: Any) -> Any
  description "Set the values of multiple channel variables or functions."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/{channelId}/variables")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
