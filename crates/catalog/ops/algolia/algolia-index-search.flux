op algolia-index-search(index_name: String, query: String, hits_per_page: Number, page: Number, filters: String) -> Any
  description "Search one index and return the matching records. Algolia ranks results by its own configured relevance, so the order is the index's, not this connector's; `hits_per_page` and `page` walk the result set, and `filters` narrows it with Algolia's filter syntax over the index's own attributes. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/message` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{app_id}.algolia.net"
  X_Algolia_Application_Id = "{app_id}"
  url = fmt("{base}/1/indexes/{index_name}")
  response = http.request(headers: { "X-Algolia-Application-Id": X_Algolia_Application_Id }, method: "GET", query: { filters, hitsPerPage: hits_per_page, page, query }, url)
  return response
