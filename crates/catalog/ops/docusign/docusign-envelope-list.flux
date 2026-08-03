op docusign-envelope-list(from_date: String) -> Any
  description "List envelopes whose status changed on or after a given date. DocuSign requires a from_date (or an explicit envelope_ids/transaction_ids list, which this operation does not offer) on every call to this resource — omitting it is a 400 from the vendor, not a limitation of this connector. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/envelopes")
  response = http.request(method: "GET", query: { from_date }, url)
  return response
