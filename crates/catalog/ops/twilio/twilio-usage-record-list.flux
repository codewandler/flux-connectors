op twilio-usage-record-list(IncludeSubaccounts: Bool, PageSize: Number, Page: Number) -> Any
  description "List usage records for the configured Twilio account with bounded pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Usage/Records.json")
  response = http.request(method: "GET", query: { IncludeSubaccounts, Page, PageSize }, url)
  return response
