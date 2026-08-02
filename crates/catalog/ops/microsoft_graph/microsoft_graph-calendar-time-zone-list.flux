op microsoft_graph-calendar-time-zone-list(_top: Number, _skip: Number) -> Any
  description "List the mailbox server's supported time zones with the Microsoft Graph MailboxSettings.Read permission"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/outlook/supportedTimeZones()")
  sep = "?"
  when _top
    url = fmt("{url}{sep}$top={_top}")
    sep = "&"
  when _skip
    url = fmt("{url}{sep}$skip={_skip}")
  response = http.request(method: "GET", url)
  return response
