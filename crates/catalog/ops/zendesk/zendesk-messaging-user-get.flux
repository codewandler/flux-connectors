op zendesk-messaging-user-get(userIdOrExternalId: String) -> Any
  description "Get one Zendesk Messaging user by its vendor id or external id"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/users/{userIdOrExternalId}")
  response = http.request(method: "GET", url)
  return response
