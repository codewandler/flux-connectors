op twilio-usage-record-list(IncludeSubaccounts: Bool, PageSize: Number, Page: Number) -> Any
  description "List usage records for the configured Twilio account with bounded pagination"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Usage/Records.json")
  sep = "?"
  when IncludeSubaccounts
    url = fmt("{url}{sep}IncludeSubaccounts={IncludeSubaccounts}")
    sep = "&"
  when PageSize
    url = fmt("{url}{sep}PageSize={PageSize}")
    sep = "&"
  when Page
    url = fmt("{url}{sep}Page={Page}")
  response = http.request(method: "GET", url)
  return response
