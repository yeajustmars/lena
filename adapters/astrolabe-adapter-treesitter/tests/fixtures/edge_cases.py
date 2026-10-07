def func_a():
    pass

class MyClass:
    def method_b(self):
        func_a()

def chained():
    obj.method_b().func_a()

def syntax_err():
    func_a()
    ] # Recoverable syntax error inside the body

# --- Enterprise Addition ---
from fastapi import APIRouter, Depends
router = APIRouter()

class UserService:
    async def get_user(self, user_id: int):
        return self.db.query(User).filter_by(id=user_id).first()

@router.post("/checkout")
async def process_checkout(payload: dict, svc: UserService = Depends()):
    with db.transaction():
        user = await svc.get_user(payload['user_id'])
        stripe.PaymentIntent.create(amount=100)
        return {"status": "ok"}
