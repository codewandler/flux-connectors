op zendesk-messaging-user-create(externalId: String) -> Any
  description "Create a Zendesk Messaging user with one required external id"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/users")
  content_type = "application/json"
  payload = { externalId }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
