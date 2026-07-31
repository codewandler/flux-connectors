op confluence-space-list -> Any
  description "List the spaces on the Confluence site, with the id, key and name of each. Takes no argument, and is this connector's `verify`: a bounded read that runs unattended and needs nothing configured beyond the credential. Returns only Confluence's first page of 25 spaces — paging needs the `cursor` and `limit` query parameters this connector cannot send, so a site with more spaces is truncated with no further signal than a `_links.next` this connector cannot follow"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{site}.atlassian.net/wiki"
  url = fmt("{base}/api/v2/spaces")
  response = http.request(method: "GET", url)
  return response
