function js_func() {
    console.log("hello");
}
js_func();

// --- Enterprise Addition ---
class CheckoutController {
    async handle(req, res) {
        await this.service.process(req.body);
    }
}

const processPayment = async (amount) => {
    return await stripe.charges.create({ amount });
};

app.post('/api/pay', async (req, res) => {
    db.table('orders').insert(req.body).returning('id');
    await processPayment(100);
    res.status(200).send();
});
