op twilio-recording-list(IncludeSoftDeleted: Bool, PageSize: Number, Page: Number) -> Any
  description "List recordings for the configured Twilio account with bounded pagination"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  AccountSid = "{username.twilio.basic_auth}"
  url = fmt("{base}/Accounts/{AccountSid}/Recordings.json")
  response = http.request(method: "GET", query: { IncludeSoftDeleted, Page, PageSize }, url)
  return response
