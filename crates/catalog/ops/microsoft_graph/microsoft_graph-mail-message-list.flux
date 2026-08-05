op microsoft_graph-mail-message-list(_top: Number, _skip: Number) -> Any
  description "List Outlook messages visible to the signed-in user with the Microsoft Graph Mail.Read permission; returns personal correspondence, so treat subjects, senders, recipients and bodies as personal data"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/messages")
  response = http.request(method: "GET", query: { "$skip": _skip, "$top": _top }, url)
  return response
