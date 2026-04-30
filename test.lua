-- sigma
class Counter 
    fn init() 
        self.val = 0 
    end 
    fn inc()
        self.val++ 
    end -- yay!
end
c = Counter()
c.inc()
c.val += 10
print("Value is", c.val) -- Value is 11
