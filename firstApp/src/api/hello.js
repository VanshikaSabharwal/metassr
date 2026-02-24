 function GET(req) {
     return JSON.stringify({
         status: 200,
         body: { message: "Hello from API" }
     });
 }

 function POST(req) {
     const data = JSON.parse(req.body || "{}");
     return JSON.stringify({
         status: 201,
         body: { received: data }
     });
 }

 module.exports = { GET, POST };