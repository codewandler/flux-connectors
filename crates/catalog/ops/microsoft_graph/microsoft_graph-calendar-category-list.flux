op microsoft_graph-calendar-category-list(_top: Number, _skip: Number) -> Any
  description "List the signed-in user's Outlook master categories with the Microsoft Graph MailboxSettings.Read permission"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/outlook/masterCategories")
  response = http.request(method: "GET", query: { "$skip": _skip, "$top": _top }, url)
  return response
