op zendesk-messaging-conversation-update(conversationId: String, displayName: String) -> Any
  description "Set one conversation's display name to an absolute non-empty value"
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}")
  content_type = "application/json"
  payload = { displayName }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
