op microsoft_graph-mail-message-list(_top: Number, _skip: Number) -> Any
  description "List Outlook messages visible to the signed-in user with the Microsoft Graph Mail.Read permission; returns personal correspondence, so treat subjects, senders, recipients and bodies as personal data"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://graph.microsoft.com"
  url = fmt("{base}/v1.0/me/messages")
  sep = "?"
  when _top
    url = fmt("{url}{sep}$top={_top}")
    sep = "&"
  when _skip
    url = fmt("{url}{sep}$skip={_skip}")
  response = http.request(method: "GET", url)
  return response
