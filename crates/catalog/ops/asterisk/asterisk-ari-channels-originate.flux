op asterisk-ari-channels-originate(endpoint: String, extension: String, context: String, priority: Number, label: String, app: String, appArgs: String, callerId: String, timeout: Number, channelId: String, otherChannelId: String, originator: String, formats: String, variables: Any) -> Any
  description "Create a new channel (originate)."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { app, appArgs, callerId, channelId, context, endpoint, extension, formats, label, originator, otherChannelId, priority, timeout: $timeout }, url)
  return response
