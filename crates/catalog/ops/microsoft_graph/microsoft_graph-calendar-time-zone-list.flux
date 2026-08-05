op microsoft_graph-calendar-time-zone-list(_top: Number, _skip: Number) -> Any
  description "List the mailbox server's supported time zones with the Microsoft Graph MailboxSettings.Read permission"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/outlook/supportedTimeZones()")
  response = http.request(method: "GET", query: { "$skip": _skip, "$top": _top }, url)
  return response
