op twilio-call-list(account_sid: String, status: String, page: Number, page_size: Number) -> Any
  description "List calls made from or received by this account"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Calls.json")
  response = http.request(method: "GET", query: { Page: page, PageSize: page_size, Status: status }, url)
  return response
