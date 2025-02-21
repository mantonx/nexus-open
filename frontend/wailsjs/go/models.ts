export namespace main {
	
	export class Config {
	    HexColor: string;
	    Unit: string;
	
	    static createFrom(source: any = {}) {
	        return new Config(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.HexColor = source["HexColor"];
	        this.Unit = source["Unit"];
	    }
	}

}

