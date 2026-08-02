op zendesk-messaging-participant-list(conversationId: String) -> Any
  description "List the participants of one conversation without exposing deep-object pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{subdomain}.zendesk.com/sc"
  appId = "{appId}"
  url = fmt("{base}/v2/apps/{appId}/conversations/{conversationId}/participants")
  response = http.request(method: "GET", url)
  return response
