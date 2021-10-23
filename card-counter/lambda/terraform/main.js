let params = new URLSearchParams(window.location.search);
console.log(params);
if (params.has("date_range")) {
  let date_range = params.get("date_range");
  let el = document.getElementById("burndown");
  let src = `https://s3.ca-central-1.amazonaws.com/card-counter.slack/burndown-${date_range}.svg`;
  el.src = src;
}
