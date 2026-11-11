<?php
class Processor {
    public function handle() {
        var_dump("Processing");
        $this->internal();
    }
    private function internal() {}
}
