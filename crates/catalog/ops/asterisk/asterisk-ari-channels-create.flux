op asterisk-ari-channels-create(endpoint: String, app: String, appArgs: String, channelId: String, otherChannelId: String, originator: String, formats: String, variables: Any) -> Any
  description "Create channel."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/create")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { app, appArgs, channelId, endpoint, formats, originator, otherChannelId }, url)
  return response
