op twilio-call-list(account_sid: String, status: String, page: Number, page_size: Number) -> Any
  description "List calls made from or received by this account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Calls.json")
  sep = "?"
  when status
    url = fmt("{url}{sep}Status={status}")
    sep = "&"
  when page
    url = fmt("{url}{sep}Page={page}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}PageSize={page_size}")
  response = http.request(method: "GET", url)
  return response
