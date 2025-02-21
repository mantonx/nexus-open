export namespace main {
	
	export class Config {
	    location: string;
	    time_format: string;
	    unit: string;
	    background_color: string;
	    text_color: string;
	    image_paths: string[];
	
	    static createFrom(source: any = {}) {
	        return new Config(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.location = source["location"];
	        this.time_format = source["time_format"];
	        this.unit = source["unit"];
	        this.background_color = source["background_color"];
	        this.text_color = source["text_color"];
	        this.image_paths = source["image_paths"];
	    }
	}
	export class ImageInfo {
	    originalName: string;
	    storedName: string;
	
	    static createFrom(source: any = {}) {
	        return new ImageInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.originalName = source["originalName"];
	        this.storedName = source["storedName"];
	    }
	}

}

