op zendesk-messaging-conversation-create -> Any
  description "Create an empty SDK group conversation in the configured Zendesk Messaging app"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations")
  content_type = "application/json"
  type = "sdkGroup"
  payload = { type }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
