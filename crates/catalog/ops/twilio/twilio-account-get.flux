op twilio-account-get(account_sid: String) -> Any
  description "Verify credentials by fetching the authenticated Twilio account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.twilio.com/2010-04-01"
  url = fmt("{base}/Accounts/{account_sid}.json")
  response = http.request(method: "GET", url)
  return response
