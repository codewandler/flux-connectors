op twilio-message-list(account_sid: String, page: Number, page_size: Number) -> Any
  description "List messages sent from or received by this account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Messages.json")
  sep = "?"
  when page
    url = fmt("{url}{sep}Page={page}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}PageSize={page_size}")
  response = http.request(method: "GET", url)
  return response
