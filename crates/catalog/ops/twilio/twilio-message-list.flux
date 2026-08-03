op twilio-message-list(account_sid: String, page: Number, page_size: Number) -> Any
  description "List messages sent from or received by this account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}/Messages.json")
  response = http.request(method: "GET", query: { Page: page, PageSize: page_size }, url)
  return response
