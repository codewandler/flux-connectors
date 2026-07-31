op docusign-envelope-recipients-get(envelope_id: String) -> Any
  description "Get every recipient on an envelope and their signing status. Recipient name and email are personal data — see this operation's own response_schema before logging, displaying or forwarding it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/envelopes/{envelope_id}/recipients")
  response = http.request(method: "GET", url)
  return response
