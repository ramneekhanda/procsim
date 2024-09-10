export const ssr = false;

function getExampleFiles(filename: string) {
  let fileurl = new URL(`./examples/${filename}`, import.meta.url).href;
  console.log(fileurl, filename);
  return fetch(fileurl)
    .then((response) => response.text())
    .then((data) => {
      code = data;
    }).catch((error) => {
      console.error('Error:', error);
    });
}
function exampleClicked(i: Object) {
  if (i.detail.filename) 
    getExampleFiles(i.detail.filename);
}