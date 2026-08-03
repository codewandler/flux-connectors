op asterisk-ari-channels-external-media(channelId: String, app: String, external_host: String, encapsulation: String, transport: String, connection_type: String, format: String, direction: String, data: String, transport_data: String, variables: Any) -> Any
  description "Start an External Media session."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/channels/externalMedia")
  content_type = "application/json"
  payload = { variables }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { app, channelId, connection_type, data, direction, encapsulation, external_host, format, transport, transport_data }, url)
  return response
