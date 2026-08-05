op docusign-envelope-get(envelope_id: String) -> Any
  description "Get one envelope's own status and metadata. No recipient or document detail — see docusign-envelope-recipients-get for who is on it. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message`, its error code at `/errorCode` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{account_host}/restapi/v2.1/accounts/{account_id}"
  url = fmt("{base}/envelopes/{envelope_id}")
  response = http.request(method: "GET", url)
  return response
