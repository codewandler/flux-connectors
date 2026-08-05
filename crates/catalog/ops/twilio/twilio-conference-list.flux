op twilio-conference-list(PageSize: Number, Page: Number) -> Any
  description "List conferences for the configured Twilio account with bounded pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Conferences.json")
  response = http.request(method: "GET", query: { Page, PageSize }, url)
  return response
