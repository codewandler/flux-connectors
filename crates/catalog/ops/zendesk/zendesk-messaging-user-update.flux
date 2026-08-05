op zendesk-messaging-user-update(userIdOrExternalId: String, toBeRetained: Bool) -> Any
  description "Set whether one Zendesk Messaging user is retained after becoming inactive"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/users/{userIdOrExternalId}")
  content_type = "application/json"
  payload = { toBeRetained }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
