op algolia-index-list -> Any
  description "List the indices in this Algolia application, with each index's record count and size. This is the call that discovers the index names every other operation here takes. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{app_id}.algolia.net"
  X_Algolia_Application_Id = "{app_id}"
  url = fmt("{base}/1/indexes")
  response = http.request(headers: { "X-Algolia-Application-Id": X_Algolia_Application_Id }, method: "GET", url)
  return response
