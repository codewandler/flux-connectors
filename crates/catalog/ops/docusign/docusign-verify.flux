op docusign-verify -> Any
  description "List the account's own top-level folders — Draft, Sent Items, Inbox, and any custom ones. Takes no parameters and succeeds for any account with API access, which is what makes it the connection check for a settings page's Test Connection button. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/folders")
  response = http.request(method: "GET", url)
  return response
