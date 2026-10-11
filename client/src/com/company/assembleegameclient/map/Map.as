package com.company.assembleegameclient.map {
import com.company.assembleegameclient.background.Background;
import com.company.assembleegameclient.game.GameSprite;
import com.company.assembleegameclient.map.mapoverlay.MapOverlay;
import com.company.assembleegameclient.map.partyoverlay.PartyOverlay;
import com.company.assembleegameclient.objects.BasicObject;
import com.company.assembleegameclient.objects.GameObject;
import com.company.assembleegameclient.objects.Party;
import com.company.assembleegameclient.objects.Player;
import com.company.assembleegameclient.parameters.Parameters;
import com.company.assembleegameclient.util.ConditionEffect;

import com.company.util.GraphicsUtil;

import flash.display.BitmapData;
import flash.display.GraphicsBitmapFill;
import flash.display.GraphicsPath;
import flash.display.GraphicsSolidFill;
import flash.display.IGraphicsData;
import flash.display.Sprite;
import flash.display3D.Context3D;
import flash.filters.BlurFilter;
import flash.filters.ColorMatrixFilter;
import flash.geom.ColorTransform;
import flash.geom.Matrix;
import flash.geom.Point;
import flash.geom.Rectangle;
import flash.utils.Dictionary;
import flash.utils.getTimer;

import kabam.rotmg.core.StaticInjectorContext;
import kabam.rotmg.stage3D.GraphicsFillExtra;
import kabam.rotmg.stage3D.Object3D.Object3DStage3D;
import kabam.rotmg.stage3D.Render3D;
import kabam.rotmg.stage3D.Renderer;
import kabam.rotmg.stage3D.Stage3DConfig;
import kabam.rotmg.stage3D.graphic3D.Program3DFactory;
import kabam.rotmg.stage3D.graphic3D.TextureFactory;

import org.osflash.signals.Signal;

public class Map extends Sprite {
    public static const NEXUS:String = "Nexus";
    private static const VISIBLE_SORT_FIELDS:Array = ["sortVal_", "objectId_"];
    private static const VISIBLE_SORT_PARAMS:Array = [Array.NUMERIC, Array.NUMERIC];
    protected static const BLIND_FILTER:ColorMatrixFilter = new ColorMatrixFilter([0.05, 0.05, 0.05, 0, 0, 0.05, 0.05, 0.05, 0, 0, 0.05, 0.05, 0.05, 0, 0, 0.05, 0.05, 0.05, 1, 0]);
    protected static var BREATH_CT:ColorTransform = new ColorTransform(255 / 255, 55 / 255, 0 / 255, 0);

    public function Map(gs:GameSprite) {
        this.map_ = new Sprite();
        this.squareList_ = new Vector.<Square>();
        this.squares_ = new Vector.<Square>();
        this.goDict_ = new Dictionary();
        this.boDict_ = new Dictionary();
        this.merchLookup_ = new Object();
        this.objsToAdd_ = new Vector.<BasicObject>();
        this.idsToRemove_ = new Vector.<int>();
        this.graphicsData_ = new Vector.<IGraphicsData>();
        this.graphicsDataStageSoftware_ = new Vector.<IGraphicsData>();
        this.graphicsData3d_ = new Vector.<Object3DStage3D>();
        this.visible_ = new Array();
        this.visibleUnder_ = new Array();
        this.visibleSquares_ = new Vector.<Square>();
        this.topSquares_ = new Vector.<Square>();
        super();
        this.gs_ = gs;
        this.hurtOverlay_ = new HurtOverlay();
        this.gradientOverlay_ = new GradientOverlay();
        this.mapOverlay_ = new MapOverlay();
        this.partyOverlay_ = new PartyOverlay(this);
        this.party_ = new Party(this);
        this.quest_ = new Quest(this);
        this.signalRenderSwitch = new Signal();
        this.hitTEnemies_ = new Vector.<GameObject>();
        this.hitTPlayers_ = new Vector.<GameObject>();
        wasLastFrameGpu = Parameters.isGpuRender();
        Parameters.GPURenderFrame = wasLastFrameGpu
    }

    public var gs_:GameSprite;
    public var width_:int;
    public var height_:int;
    public var name_:String;
    public var back_:int;
    public var allowPlayerTeleport_:Boolean;
    public var showDisplays_:Boolean;
    public var background_:Background = null;
    public var map_:Sprite;
    public var hurtOverlay_:HurtOverlay = null;
    public var gradientOverlay_:GradientOverlay = null;
    public var mapOverlay_:MapOverlay = null;
    public var partyOverlay_:PartyOverlay = null;
    public var squareList_:Vector.<Square>;
    public var squares_:Vector.<Square>;
    public var goDict_:Dictionary;
    public var boDict_:Dictionary;
    public var merchLookup_:Object;
    public var player_:Player = null;
    public var party_:Party = null;
    public var quest_:Quest = null;
    public var visible_:Array;
    public var visibleUnder_:Array;
    public var visibleSquares_:Vector.<Square>;
    public var topSquares_:Vector.<Square>;
    public var signalRenderSwitch:Signal;
    public var wasLastFrameGpu:Boolean = false;
    // good projectile optimization
    public var hitTEnemies_:Vector.<GameObject>;
    public var hitTPlayers_:Vector.<GameObject>;
    private var inUpdate_:Boolean = false;
    private var objsToAdd_:Vector.<BasicObject>;
    private var idsToRemove_:Vector.<int>;
    private var graphicsData_:Vector.<IGraphicsData>;
    private var graphicsDataStageSoftware_:Vector.<IGraphicsData>;
    private var graphicsData3d_:Vector.<Object3DStage3D>;
    private var lastSoftwareClear:Boolean = false;

    public var disableShooting_:Boolean;
    public var disableAbilities_:Boolean;

    // Ground chunks. Static ground tiles (Square.chunkable_) are copied into one 128x128 image
    // per 16x16 tiles, and each visible chunk is drawn as a single textured quad (one GPU draw
    // call) instead of up to 256 tile faces. A chunk is rebuilt when a tile in it or next to it
    // changes (setGroundTile), and freed when it has not been drawn for CHUNK_KEEP_MS.
    private static const CHUNK:int = 16;
    private static const TEX:int = 8;
    private static const CHUNK_KEEP_MS:int = 10000;
    // Building a chunk also prepares its tiles (edge blending), so only a few are built per
    // frame: rotating the camera brings many new chunks into view at once.
    private static const CHUNK_BUILDS_PER_FRAME:int = 2;
    private static const chunkPoint_:Point = new Point();
    private static const chunkRect_:Rectangle = new Rectangle();
    private var chunksW_:int = 0;
    private var chunks_:Vector.<BitmapData> = null;
    private var chunkDirty_:Vector.<Boolean> = null;
    private var chunkUsed_:Vector.<int> = null;
    // whether a chunk drawn this frame is built; if not, its tiles are drawn one by one
    private var chunkReady_:Vector.<Boolean> = null;
    private var chunkBuilds_:int = 0;
    private var chunkFills_:Vector.<GraphicsBitmapFill> = null;
    private var chunkPaths_:Vector.<GraphicsPath> = null;
    private var chunksAllocated_:Vector.<int> = new Vector.<int>();
    private var chunksVisible_:Vector.<int> = new Vector.<int>();
    private var lastChunkSweep_:int = 0;
    // World-to-map-pixel transform of the ground (z = 0) this frame: origin and the screen
    // vectors of one tile along world x and y. groundAffine_ is false for the perspective
    // camera, which draws every tile the original way.
    private var groundAffine_:Boolean = false;
    private var groundOx_:Number = 0;
    private var groundOy_:Number = 0;
    private var groundXx_:Number = 0;
    private var groundXy_:Number = 0;
    private var groundYx_:Number = 0;
    private var groundYy_:Number = 0;
    private var lastDrawTime_:int = -1;
    // Performance Stats: time in each part of draw() since GameStatistics last read them
    public static var profTiles:int = 0;
    private var ownTiles_:Vector.<Square> = new Vector.<Square>();
    public static var profCollect:int = 0;
    public static var profObjects:int = 0;
    public static var chunking:Boolean = true; // debug switches
    public static var culling:Boolean = true;

    private function setupGround(camera:Camera):void {
        // Needs the orthographic camera, where a ground point (x, y, 0) lands at
        // (x * r0 + y * r4 + r12, x * r1 + y * r5 + r13).
        this.groundAffine_ = chunking && camera.affineVersion_ != 0 && this.width_ > 0;
        if (!this.groundAffine_) {
            return;
        }
        if (this.chunks_ == null) {
            this.chunksW_ = Math.ceil(this.width_ / CHUNK);
            var n:int = this.chunksW_ * Math.ceil(this.height_ / CHUNK);
            this.chunks_ = new Vector.<BitmapData>(n);
            this.chunkDirty_ = new Vector.<Boolean>(n);
            this.chunkUsed_ = new Vector.<int>(n);
            this.chunkReady_ = new Vector.<Boolean>(n);
            this.chunkFills_ = new Vector.<GraphicsBitmapFill>(n);
            this.chunkPaths_ = new Vector.<GraphicsPath>(n);
        }
        // read straight from the camera matrix, the same numbers walls and objects use
        var r:Vector.<Number> = camera.wToSRaw_;
        this.groundOx_ = r[12];
        this.groundOy_ = r[13];
        this.groundXx_ = r[0];
        this.groundXy_ = r[1];
        this.groundYx_ = r[4];
        this.groundYy_ = r[5];
    }

    private function markChunksDirty(x:int, y:int):void {
        if (this.chunks_ == null) {
            return;
        }
        // the tile's neighbours blend with it, so their chunks change too
        for (var xi:int = Math.max(0, x - 1); xi <= Math.min(this.width_ - 1, x + 1); xi++) {
            for (var yi:int = Math.max(0, y - 1); yi <= Math.min(this.height_ - 1, y + 1); yi++) {
                this.chunkDirty_[int(xi / CHUNK) + int(yi / CHUNK) * this.chunksW_] = true;
            }
        }
    }

    // Makes sure the chunk image is up to date, within this frame's build budget. Returns false
    // when it still needs building; it is tried again next frame.
    private function prepareChunk(c:int):Boolean {
        var bmp:BitmapData = this.chunks_[c];
        if (bmp != null && !this.chunkDirty_[c]) {
            return true;
        }
        if (this.chunkBuilds_ >= CHUNK_BUILDS_PER_FRAME) {
            return false;
        }
        this.chunkBuilds_++;
        if (bmp != null) {
            // a new image rather than redrawing this one: the GPU caches textures by image
            this.freeChunkImage(c);
        }
        else {
            this.chunksAllocated_.push(c);
        }
        bmp = new BitmapData(CHUNK * TEX, CHUNK * TEX, true, 0);
        this.chunks_[c] = bmp;
        this.buildChunk(c, bmp);
        return true;
    }

    private function freeChunkImage(c:int):void {
        var bmp:BitmapData = this.chunks_[c];
        if (bmp == null) {
            return;
        }
        TextureFactory.disposeTexture(bmp);
        bmp.dispose();
        this.chunks_[c] = null;
        if (this.chunkFills_[c] != null) {
            this.chunkFills_[c].bitmapData = null;
        }
    }

    private function drawChunk(c:int):void {
        var bmp:BitmapData = this.chunks_[c];
        var x0:int = c % this.chunksW_ * CHUNK;
        var y0:int = int(c / this.chunksW_) * CHUNK;
        var fill:GraphicsBitmapFill = this.chunkFills_[c];
        var path:GraphicsPath = this.chunkPaths_[c];
        if (fill == null) {
            fill = new GraphicsBitmapFill(bmp, new Matrix(), false, false);
            path = new GraphicsPath(new <int>[1, 2, 2, 2], new Vector.<Number>(8));
            this.chunkFills_[c] = fill;
            this.chunkPaths_[c] = path;
        }
        fill.bitmapData = bmp;
        var m:Matrix = fill.matrix;
        m.a = this.groundXx_ / TEX;
        m.b = this.groundXy_ / TEX;
        m.c = this.groundYx_ / TEX;
        m.d = this.groundYy_ / TEX;
        m.tx = this.groundOx_ + x0 * this.groundXx_ + y0 * this.groundYx_;
        m.ty = this.groundOy_ + x0 * this.groundXy_ + y0 * this.groundYy_;
        fill.matrix = m;
        var d:Vector.<Number> = path.data;
        d[0] = m.tx;
        d[1] = m.ty;
        d[2] = m.tx + CHUNK * this.groundXx_;
        d[3] = m.ty + CHUNK * this.groundXy_;
        d[4] = m.tx + CHUNK * (this.groundXx_ + this.groundYx_);
        d[5] = m.ty + CHUNK * (this.groundXy_ + this.groundYy_);
        d[6] = m.tx + CHUNK * this.groundYx_;
        d[7] = m.ty + CHUNK * this.groundYy_;
        this.graphicsData_.push(fill);
        this.graphicsData_.push(path);
        this.graphicsData_.push(GraphicsUtil.END_FILL);
    }

    // Copies each chunkable tile's texture into the chunk image with copyPixels.
    private function buildChunk(c:int, bmp:BitmapData):void {
        var square:Square = null;
        var x0:int = c % this.chunksW_ * CHUNK;
        var y0:int = int(c / this.chunksW_) * CHUNK;
        bmp.lock();
        for (var yi:int = 0; yi < CHUNK && y0 + yi < this.height_; yi++) {
            for (var xi:int = 0; xi < CHUNK && x0 + xi < this.width_; xi++) {
                square = this.squares_[x0 + xi + (y0 + yi) * this.width_];
                if (square == null) {
                    continue;
                }
                square.prepareFaces();
                if (square.chunkable_) {
                    this.copyTile(bmp, square.groundTex_, xi * TEX, yi * TEX, (square.groundDx_ % TEX + TEX) % TEX, (square.groundDy_ % TEX + TEX) % TEX);
                }
                else if (square.texture_ != null && square.texture_.width == TEX && square.texture_.height == TEX) {
                    // Drawn on its own on top (e.g. animated water), but its plain texture goes
                    // underneath too, so the separate tile's edge blends into the same colour
                    // instead of the background (a dark line along every coast).
                    this.copyTile(bmp, square.texture_, xi * TEX, yi * TEX, 0, 0);
                }
            }
        }
        bmp.unlock();
        this.chunkDirty_[c] = false;
    }

    // Copies an 8x8 texture to (x, y) starting at texel (sx, sy) and wrapping around, which is
    // how a tile with a texture offset is drawn (its bitmap fill repeats).
    private function copyTile(dst:BitmapData, src:BitmapData, x:int, y:int, sx:int, sy:int):void {
        this.copyPart(dst, src, sx, sy, TEX - sx, TEX - sy, x, y);
        this.copyPart(dst, src, 0, sy, sx, TEX - sy, x + TEX - sx, y);
        this.copyPart(dst, src, sx, 0, TEX - sx, sy, x, y + TEX - sy);
        this.copyPart(dst, src, 0, 0, sx, sy, x + TEX - sx, y + TEX - sy);
    }

    private function copyPart(dst:BitmapData, src:BitmapData, sx:int, sy:int, w:int, h:int, x:int, y:int):void {
        if (w <= 0 || h <= 0) {
            return;
        }
        chunkRect_.x = sx;
        chunkRect_.y = sy;
        chunkRect_.width = w;
        chunkRect_.height = h;
        chunkPoint_.x = x;
        chunkPoint_.y = y;
        dst.copyPixels(src, chunkRect_, chunkPoint_);
    }

    // Frees chunk images that haven't been drawn for CHUNK_KEEP_MS (checked every second).
    private function sweepChunks(time:int):void {
        if (time - this.lastChunkSweep_ < 1000) {
            return;
        }
        this.lastChunkSweep_ = time;
        var i:int = 0;
        while (i < this.chunksAllocated_.length) {
            var c:int = this.chunksAllocated_[i];
            if (time - this.chunkUsed_[c] > CHUNK_KEEP_MS) {
                this.freeChunkImage(c);
                var last:int = this.chunksAllocated_.length - 1;
                this.chunksAllocated_[i] = this.chunksAllocated_[last];
                this.chunksAllocated_.length = last;
            }
            else {
                i++;
            }
        }
    }

    private function disposeChunks():void {
        if (this.chunks_ != null) {
            for each(var c:int in this.chunksAllocated_) {
                this.freeChunkImage(c);
            }
        }
        this.chunksAllocated_.length = 0;
        this.chunks_ = null;
        this.chunkFills_ = null;
        this.chunkPaths_ = null;
    }

    // Only tiles within the view radius of the player are drawn (the server forgets the walls and
    // objects further out), but a chunk image holds all its remembered tiles. The parts of the
    // visible chunks outside that circle are covered with black, one strip per row segment
    // (all batched into one GPU draw call), drawn right after the chunks and under everything else.
    public static const MIN_VIEW_RADIUS:int = 15; // the server's default (Player.VISIBILITY_RADIUS)
    public static const MAX_VIEW_RADIUS:int = 40; // its maximum (Player.MAX_VISIBILITY_RADIUS)
    // the radius the view needs this frame (GameSprite sends it to the server when it changes)
    public var viewRadius_:int = MIN_VIEW_RADIUS;
    private var boundX0_:int = 0;
    private var boundX1_:int = 0;
    private var boundY0_:int = 0;
    private var boundY1_:int = 0;

    // The world tiles under the screen rectangle (l, t, r, b in map pixels, margins included):
    // their bounding box, and the distance from the player to its farthest corner.
    private function viewBounds(l:Number, t:Number, r:Number, b:Number, px:int, py:int):void {
        var det:Number = this.groundXx_ * this.groundYy_ - this.groundYx_ * this.groundXy_;
        if (det == 0) {
            return;
        }
        var minX:Number = Number.MAX_VALUE, minY:Number = Number.MAX_VALUE;
        var maxX:Number = -Number.MAX_VALUE, maxY:Number = -Number.MAX_VALUE;
        var far:Number = 0;
        for (var i:int = 0; i < 4; i++) {
            var sx:Number = (i & 1) == 0 ? l : r;
            var sy:Number = (i & 2) == 0 ? t : b;
            var wx:Number = ((sx - this.groundOx_) * this.groundYy_ - (sy - this.groundOy_) * this.groundYx_) / det;
            var wy:Number = ((sy - this.groundOy_) * this.groundXx_ - (sx - this.groundOx_) * this.groundXy_) / det;
            minX = Math.min(minX, wx);
            maxX = Math.max(maxX, wx);
            minY = Math.min(minY, wy);
            maxY = Math.max(maxY, wy);
            far = Math.max(far, Math.sqrt((wx - px) * (wx - px) + (wy - py) * (wy - py)));
        }
        this.boundX0_ = Math.floor(minX);
        this.boundX1_ = Math.ceil(maxX);
        this.boundY0_ = Math.floor(minY);
        this.boundY1_ = Math.ceil(maxY);
        this.viewRadius_ = Math.max(MIN_VIEW_RADIUS, Math.min(MAX_VIEW_RADIUS, Math.ceil(far)));
    }
    private static var maskTexture_:BitmapData = null;
    private var maskFills_:Vector.<GraphicsBitmapFill> = new Vector.<GraphicsBitmapFill>();
    private var maskPaths_:Vector.<GraphicsPath> = new Vector.<GraphicsPath>();

    private function maskChunks(px:int, py:int, radius:int):void {
        if (maskTexture_ == null) {
            maskTexture_ = new BitmapData(TEX, TEX, false, 0);
        }
        var used:int = 0;
        var r2:int = radius * radius;
        for each(var c:int in this.chunksVisible_) {
            var x0:int = c % this.chunksW_ * CHUNK;
            var y0:int = int(c / this.chunksW_) * CHUNK;
            var x1:int = x0 + CHUNK - 1;
            for (var y:int = y0; y < y0 + CHUNK; y++) {
                var dy:int = y - py;
                if (dy * dy > r2) {
                    used = this.maskRun(used, x0, x1, y);
                    continue;
                }
                // the row's tiles inside the circle are px - hw .. px + hw (the loop in draw())
                var hw:int = int(Math.sqrt(r2 - dy * dy));
                if (x0 < px - hw) {
                    used = this.maskRun(used, x0, Math.min(x1, px - hw - 1), y);
                }
                if (x1 > px + hw) {
                    used = this.maskRun(used, Math.max(x0, px + hw + 1), x1, y);
                }
            }
        }
    }

    // A black strip over tiles xa..xb of row y; returns the number of strips used this frame.
    private function maskRun(used:int, xa:int, xb:int, y:int):int {
        if (xb < xa) {
            return used;
        }
        if (used == this.maskFills_.length) {
            this.maskFills_.push(new GraphicsBitmapFill(maskTexture_, new Matrix(), false, false));
            this.maskPaths_.push(new GraphicsPath(new <int>[1, 2, 2, 2], new Vector.<Number>(8)));
        }
        var fill:GraphicsBitmapFill = this.maskFills_[used];
        var path:GraphicsPath = this.maskPaths_[used];
        var w:int = xb - xa + 1;
        var m:Matrix = fill.matrix;
        m.a = this.groundXx_ * w / TEX;
        m.b = this.groundXy_ * w / TEX;
        m.c = this.groundYx_ / TEX;
        m.d = this.groundYy_ / TEX;
        m.tx = this.groundOx_ + xa * this.groundXx_ + y * this.groundYx_;
        m.ty = this.groundOy_ + xa * this.groundXy_ + y * this.groundYy_;
        fill.matrix = m;
        var d:Vector.<Number> = path.data;
        d[0] = m.tx;
        d[1] = m.ty;
        d[2] = m.tx + w * this.groundXx_;
        d[3] = m.ty + w * this.groundXy_;
        d[4] = m.tx + w * this.groundXx_ + this.groundYx_;
        d[5] = m.ty + w * this.groundXy_ + this.groundYy_;
        d[6] = m.tx + this.groundYx_;
        d[7] = m.ty + this.groundYy_;
        this.graphicsData_.push(fill);
        this.graphicsData_.push(path);
        this.graphicsData_.push(GraphicsUtil.END_FILL);
        return used + 1;
    }

    // Fills in a chunked tile's projected corners (Face3D.vout_), for objects drawn on it.
    private function projectSquare(square:Square):void {
        if (!this.groundAffine_ || square == null || !square.chunkable_ || square.faces_.length == 0) {
            return;
        }
        var v:Vector.<Number> = square.faces_[0].face_.vout_;
        var x:Number = this.groundOx_ + square.x_ * this.groundXx_ + square.y_ * this.groundYx_;
        var y:Number = this.groundOy_ + square.x_ * this.groundXy_ + square.y_ * this.groundYy_;
        v.length = 8;
        v[0] = x;
        v[1] = y;
        v[2] = x + this.groundXx_;
        v[3] = y + this.groundXy_;
        v[4] = x + this.groundXx_ + this.groundYx_;
        v[5] = y + this.groundXy_ + this.groundYy_;
        v[6] = x + this.groundYx_;
        v[7] = y + this.groundYy_;
    }

    public function setProps(width:int, height:int, name:String, back:int, allowPlayerTeleport:Boolean, showDisplays:Boolean, disableShooting:Boolean, disableAbilities:Boolean):void {
        this.width_ = width;
        this.height_ = height;
        this.name_ = name;
        this.back_ = back;
        this.allowPlayerTeleport_ = allowPlayerTeleport;
        this.showDisplays_ = showDisplays;
        this.disableShooting_ = disableShooting;
        this.disableAbilities_ = disableAbilities;
    }

    public function initialize():void {
        this.squares_.length = this.width_ * this.height_;
        this.background_ = Background.getBackground(this.back_);
        if (this.background_ != null) {
            addChild(this.background_);
        }
        addChild(this.map_);
        addChild(this.hurtOverlay_);
        addChild(this.gradientOverlay_);
        addChild(this.mapOverlay_);
        addChild(this.partyOverlay_);
    }

    public function dispose():void {
        var square:Square = null;
        var go:GameObject = null;
        var bo:BasicObject = null;
        this.gs_ = null;
        this.background_ = null;
        this.map_ = null;
        this.hurtOverlay_ = null;
        this.gradientOverlay_ = null;
        this.mapOverlay_ = null;
        this.partyOverlay_ = null;
        for each(square in this.squareList_) {
            square.dispose();
        }
        this.squareList_.length = 0;
        this.squareList_ = null;
        this.disposeChunks();
        this.squares_.length = 0;
        this.squares_ = null;
        for each(go in this.goDict_) {
            go.dispose();
        }
        this.goDict_ = null;
        for each(bo in this.boDict_) {
            bo.dispose();
        }
        this.boDict_ = null;
        this.merchLookup_ = null;
        this.player_ = null;
        this.party_ = null;
        this.quest_ = null;
        this.objsToAdd_ = null;
        this.idsToRemove_ = null;
        this.hitTPlayers_.length = 0;
        this.hitTEnemies_.length = 0;
        this.hitTPlayers_ = null;
        this.hitTEnemies_ = null;
        TextureFactory.disposeTextures();
        GraphicsFillExtra.dispose();
        Program3DFactory.getInstance().dispose();
    }

    public function update(time:int, dt:int):void {
        var bo:BasicObject = null;
        var go:GameObject = null;
        var objId:int = 0;
        this.inUpdate_ = true;

        this.hitTPlayers_.length = 0;
        this.hitTEnemies_.length = 0;
        for each(go in this.goDict_) {
            if (!go.update(time, dt)) {
                this.idsToRemove_.push(go.objectId_);
            }
            else {
                if (go.props_.isEnemy_) {
                    if (!go.isUntargetable()) {
                        this.hitTEnemies_.push(go);
                    }
                }
                else if (go.props_.isPlayer_) {
                    if (!go.isUntargetable()) {
                        this.hitTPlayers_.push(go);
                    }
                }
            }
        }
        for each(bo in this.boDict_) {
            if (!bo.update(time, dt)) {
                this.idsToRemove_.push(bo.objectId_);
            }
        }
        this.inUpdate_ = false;
        for each(bo in this.objsToAdd_) {
            this.internalAddObj(bo);
        }
        this.objsToAdd_.length = 0;
        for each(objId in this.idsToRemove_) {
            this.internalRemoveObj(objId);
        }
        this.idsToRemove_.length = 0;
        this.party_.update(time, dt);
    }

    public function pSTopW(xS:Number, yS:Number):Point {
        var square:Square = null;
        if (this.groundAffine_) {
            // chunked tiles have no projected faces: invert the ground transform instead
            var det:Number = this.groundXx_ * this.groundYy_ - this.groundYx_ * this.groundXy_;
            if (det != 0) {
                var wx:Number = ((xS - this.groundOx_) * this.groundYy_ - (yS - this.groundOy_) * this.groundYx_) / det;
                var wy:Number = ((yS - this.groundOy_) * this.groundXx_ - (xS - this.groundOx_) * this.groundXy_) / det;
                if (wx >= 0 && wy >= 0 && wx < this.width_ && wy < this.height_) {
                    square = this.squares_[int(wx) + int(wy) * this.width_];
                    if (square != null && square.texture_ != null && square.lastVisible_ == this.lastDrawTime_) {
                        return new Point(square.center_.x, square.center_.y);
                    }
                }
            }
            return null;
        }
        for each(square in this.visibleSquares_) {
            if (square.faces_.length != 0 && square.faces_[0].face_.contains(xS, yS)) {
                return new Point(square.center_.x, square.center_.y);
            }
        }
        return null;
    }

    public function setGroundTile(x:int, y:int, tileType:uint):void {
        var yi:int = 0;
        var ind:int = 0;
        var n:Square = null;
        var square:Square = this.getSquare(x, y);
        square.setTileType(tileType);
        this.markChunksDirty(x, y);
        var xend:int = x < this.width_ - 1 ? int(x + 1) : int(x);
        var yend:int = y < this.height_ - 1 ? int(y + 1) : int(y);
        for (var xi:int = x > 0 ? int(x - 1) : int(x); xi <= xend; xi++) {
            for (yi = y > 0 ? int(y - 1) : int(y); yi <= yend; yi++) {
                ind = xi + yi * this.width_;
                n = this.squares_[ind];
                if (n != null && (n.props_.hasEdge_ || n.tileType_ != tileType)) {
                    n.faces_.length = 0;
                }
            }
        }
    }

    public function addObj(bo:BasicObject, posX:Number, posY:Number):void {
        bo.x_ = posX;
        bo.y_ = posY;
        if (this.inUpdate_) {
            this.objsToAdd_.push(bo);
        }
        else {
            this.internalAddObj(bo);
        }
    }

    public function internalAddObj(bo:BasicObject):void {
        if (!bo.addTo(this, bo.x_, bo.y_)) {
            trace("ERROR: adding: " + bo);
            return;
        }
        var dict:Dictionary = bo is GameObject ? this.goDict_ : this.boDict_;
        if (dict[bo.objectId_] != null) {
            trace("ERROR: duplicate add: " + bo + " would replace: " + dict[bo.objectId_]);
            return;
        }
        dict[bo.objectId_] = bo;
    }

    public function removeObj(objectId:int):void {
        if (this.inUpdate_) {
            this.idsToRemove_.push(objectId);
        }
        else {
            this.internalRemoveObj(objectId);
        }
    }

    public function internalRemoveObj(objectId:int):void {
        var dict:Dictionary = this.goDict_;
        var bo:BasicObject = dict[objectId];
        if (bo == null) {
            dict = this.boDict_;
            bo = dict[objectId];
            if (bo == null) {
                return;
            }
        }
        bo.removeFromMap();
        delete dict[objectId];
    }

    public function getSquare(posX:Number, posY:Number):Square {
        if (posX < 0 || posX >= this.width_ || posY < 0 || posY >= this.height_) {
            return null;
        }
        var ind:int = int(posX) + int(posY) * this.width_;
        var square:Square = this.squares_[ind];
        if (square == null) {
            square = new Square(this, int(posX), int(posY));
            this.squares_[ind] = square;
            this.squareList_.push(square);
        }
        return square;
    }

    public function lookupSquare(x:int, y:int):Square {
        if (x < 0 || x >= this.width_ || y < 0 || y >= this.height_) {
            return null;
        }
        return this.squares_[x + y * this.width_];
    }

    public function correctMapView(camera:Camera):Point {
        var clipRect:Rectangle = camera.clipRect_;
        x = ((-(clipRect.x) * 800) / (WebMain.sWidth / Parameters.data_.mscale));
        y = ((-(clipRect.y) * 600) / (WebMain.sHeight / Parameters.data_.mscale));
        var clipWidth:Number = ((-(clipRect.x) - (clipRect.width / 2)) / 50);
        var clipHeight:Number = ((-(clipRect.y) - (clipRect.height / 2)) / 50);
        var clipSqrt:Number = Math.sqrt(((clipWidth * clipWidth) + (clipHeight * clipHeight)));
        var clipAngle:Number = ((camera.angleRad_ - (Math.PI / 2)) - Math.atan2(clipWidth, clipHeight));
        return (new Point((camera.x_ + (clipSqrt * Math.cos(clipAngle))), (camera.y_ + (clipSqrt * Math.sin(clipAngle)))));
    }

    public function draw(camera:Camera, time:int):void {
        var profStart:int = getTimer();
        var isGpuRender:Boolean = Parameters.isGpuRender(); // cache result for faster access
        Parameters.GPURenderFrame = isGpuRender;
        if (this.wasLastFrameGpu) {
            WebMain.STAGE.stage3Ds[0].x = 400 - Stage3DConfig.HALF_WIDTH * Parameters.data_.mscale;
            WebMain.STAGE.stage3Ds[0].y = 300 - Stage3DConfig.HALF_HEIGHT * Parameters.data_.mscale;
        }
        if (wasLastFrameGpu != isGpuRender) {
            var context:Context3D = WebMain.STAGE.stage3Ds[0].context3D;
            if (wasLastFrameGpu && context != null && context.driverInfo.toLowerCase().indexOf("disposed") == -1) {
                context.clear();
                context.present();
            }
            else {
                map_.graphics.clear();
            }
            signalRenderSwitch.dispatch(wasLastFrameGpu);
            wasLastFrameGpu = isGpuRender;
        }

        var filter:uint = 0;
        var render3D:Render3D = null;
        var i:int = 0;
        var _loc6_:* = null;
        var square:Square = null;
        var go:GameObject = null;
        var bo:BasicObject = null;
        var yi:int = 0;
        var dX:Number = NaN;
        var dY:Number = NaN;
        var distSq:Number = NaN;
        var b:Number = NaN;
        var t:Number = NaN;
        var d:Number = NaN;
        var screenRect:Rectangle = camera.clipRect_;
        x = -screenRect.x * 800 / WebMain.sWidth;
        y = -screenRect.y * 600 / WebMain.sHeight;
        var fsPos:Point;
        var plrPos:Point = this.correctMapView(camera);
        fsPos = plrPos;
        if (Parameters.isGpuRender()) {
            _loc6_ = WebMain.STAGE.stage3Ds[0];
            _loc6_.x = 400 - WebMain.STAGE.stageWidth / 2;
            _loc6_.y = 300 - WebMain.STAGE.stageHeight / 2;
        }
        if (this.background_ != null) {
            this.background_.draw(camera, time);
        }

        this.visible_.length = 0;
        this.visibleUnder_.length = 0;
        this.visibleSquares_.length = 0;
        this.topSquares_.length = 0;

        var delta:int = camera.maxDist_;
        var xStart:int = Math.max(0, fsPos.x - delta);
        var xEnd:int = Math.min(this.width_ - 1, fsPos.x + delta);
        var yStart:int = Math.max(0, fsPos.y - delta);
        var yEnd:int = Math.min(this.height_ - 1, fsPos.y + delta);

        this.graphicsData_.length = 0;
        this.graphicsDataStageSoftware_.length = 0;
        this.graphicsData3d_.length = 0;

        // visible tiles: the circle around the player, minus what is off screen (tile centres
        // more than 2 tiles beside or above the view, or 4 below it, where a tall sprite
        // standing on the tile could still reach into view)
        this.lastDrawTime_ = time;
        this.chunkBuilds_ = 0;
        this.setupGround(camera);
        this.chunksVisible_.length = 0;
        var cullL:Number = screenRect.x - 100;
        var cullR:Number = screenRect.x + screenRect.width + 100;
        var cullT:Number = screenRect.y - 100;
        var cullB:Number = screenRect.y + screenRect.height + 200;
        var sx:Number = 0;
        var sy:Number = 0;
        var c:int = 0;
        // Draw out to the radius the view needs (the server sends that far once GameSprite tells
        // it, see viewRadius_), and only walk the tiles in the screen's bounding box.
        var px:int = int(player_.x_);
        var py:int = int(player_.y_);
        var radius:int = MIN_VIEW_RADIUS;
        var bx0:int = -MIN_VIEW_RADIUS;
        var bx1:int = MIN_VIEW_RADIUS;
        var by0:int = -MIN_VIEW_RADIUS;
        var by1:int = MIN_VIEW_RADIUS;
        if (this.groundAffine_ && culling) {
            this.viewBounds(cullL, cullT, cullR, cullB, px, py);
            radius = this.viewRadius_;
            bx0 = Math.max(-radius, this.boundX0_ - px);
            bx1 = Math.min(radius, this.boundX1_ - px);
            by0 = Math.max(-radius, this.boundY0_ - py);
            by1 = Math.min(radius, this.boundY1_ - py);
        }
        else {
            this.viewRadius_ = MIN_VIEW_RADIUS;
        }
        var r2:int = radius * radius;
        for (var xi:int = bx0; xi <= bx1; xi++) {
            for (yi = by0; yi <= by1; yi++) {
                if (xi * xi + yi * yi <= r2) {
                    square = lookupSquare(xi + px, yi + py);
                    if (square != null) {
                        if (this.groundAffine_ && culling) {
                            sx = this.groundOx_ + square.center_.x * this.groundXx_ + square.center_.y * this.groundYx_;
                            sy = this.groundOy_ + square.center_.x * this.groundXy_ + square.center_.y * this.groundYy_;
                            if (sx < cullL || sx > cullR || sy < cullT || sy > cullB) {
                                continue;
                            }
                            square.prepareFaces();
                        }
                        square.lastVisible_ = time;
                        if (this.groundAffine_ && square.chunkable_) {
                            c = int(square.x_ / CHUNK) + int(square.y_ / CHUNK) * this.chunksW_;
                            if (this.chunkUsed_[c] != time) {
                                this.chunkUsed_[c] = time;
                                this.chunkReady_[c] = this.prepareChunk(c);
                                if (this.chunkReady_[c]) {
                                    this.chunksVisible_.push(c);
                                }
                            }
                        }
                        this.visibleSquares_.push(square);
                        if (square.topFace_ != null) {
                            this.topSquares_.push(square);
                        }
                    }
                }
            }
        }
        // chunks first, under the tiles drawn on their own (animated water, edges...)
        for each(c in this.chunksVisible_) {
            this.drawChunk(c);
        }
        if (this.chunksVisible_.length > 0) {
            this.maskChunks(px, py, radius);
        }
        // the tiles not in a ready chunk: all base faces, then all overlays (see Square.drawBase)
        var own:Vector.<Square> = this.ownTiles_;
        own.length = 0;
        for each(square in this.visibleSquares_) {
            if (!(this.groundAffine_ && square.chunkable_ && this.chunkReady_[int(square.x_ / CHUNK) + int(square.y_ / CHUNK) * this.chunksW_])) {
                own.push(square);
                square.drawBase(this.graphicsData_, camera, time);
            }
        }
        for each(square in own) {
            square.drawOverlays(this.graphicsData_, camera, time);
        }
        if (this.chunks_ != null) {
            this.sweepChunks(time);
        }

//      for(var xi:int = xStart; xi <= xEnd; xi++)
//      {
//         for(yi = yStart; yi <= yEnd; yi++)
//         {
//            square = this.squares_[xi + yi * this.width_];
//            if(square != null)
//            {
//               dX = fsPos.x - square.center_.x;
//               dY = fsPos.y - square.center_.y;
//               distSq = dX * dX + dY * dY;
//               if(distSq <= camera.maxDistSq_)
//               {
//                  square.lastVisible_ = time;
//                  square.draw(this.graphicsData_,camera,time);
//                  this.visibleSquares_.push(square);
//                  if(square.topFace_ != null)
//                  {
//                     this.topSquares_.push(square);
//                  }
//               }
//            }
//         }
//      }

        var profT:int = getTimer();
        profTiles += profT - profStart;
        // visible game objects
        for each(go in this.goDict_) {
            go.drawn_ = false;
            if (!go.dead_) {
                square = go.square_;
                if (!(square == null || square.lastVisible_ != time)) {
                    go.drawn_ = true;
                    if (go.props_.drawOnGround_ || go.obj3D_ != null) {
                        // drawn using its tile's projected corners, which a chunked tile doesn't compute
                        this.projectSquare(square);
                    }
                    go.computeSortVal(camera);
                    if (go.props_.drawUnder_) {
                        if (go.props_.drawOnGround_) {
                            go.draw(this.graphicsData_, camera, time);
                        }
                        else {
                            this.visibleUnder_.push(go);
                        }
                    }
                    else {
                        this.visible_.push(go);
                    }
                }
            }
        }

        // visible basic objects (projectiles, particles and such)
        for each(bo in this.boDict_) {
            bo.drawn_ = false;
            square = bo.square_;
            if (!(square == null || square.lastVisible_ != time)) {
                bo.drawn_ = true;
                bo.computeSortVal(camera);
                this.visible_.push(bo);
            }
        }

        var profT2:int = getTimer();
        profCollect += profT2 - profT;
        // draw visible under
        if (this.visibleUnder_.length > 0) {
            this.visibleUnder_.sortOn(VISIBLE_SORT_FIELDS, VISIBLE_SORT_PARAMS);
            for each(bo in this.visibleUnder_) {
                bo.draw(this.graphicsData_, camera, time);
            }
        }

        // draw shadows
        this.visible_.sortOn(VISIBLE_SORT_FIELDS, VISIBLE_SORT_PARAMS);
        if (Parameters.data_.drawShadows) {
            for each(bo in this.visible_) {
                if (bo.hasShadow_) {
                    bo.drawShadow(this.graphicsData_, camera, time);
                }
            }
        }

        // draw visible
        for each(bo in this.visible_) {
            bo.draw(this.graphicsData_, camera, time);
            if (isGpuRender) {
                bo.draw3d(this.graphicsData3d_);
            }
        }

        // draw top squares
        if (this.topSquares_.length > 0) {
            for each(square in this.topSquares_) {
                square.drawTop(this.graphicsData_, camera, time);
            }
        }

        // draw breath overlay
        if (this.player_ != null && this.player_.breath_ >= 0 && this.player_.breath_ < Parameters.BREATH_THRESH) {
            b = (Parameters.BREATH_THRESH - this.player_.breath_) / Parameters.BREATH_THRESH;
            t = Math.abs(Math.sin(time / 300)) * 0.75;
            BREATH_CT.alphaMultiplier = b * t;
            this.hurtOverlay_.transform.colorTransform = BREATH_CT;
            this.hurtOverlay_.visible = true;
            this.hurtOverlay_.x = screenRect.left;
            this.hurtOverlay_.y = screenRect.top;
        }
        else {
            this.hurtOverlay_.visible = false;
        }

        // draw side bar gradient
        if (this.player_ != null) {
            this.gradientOverlay_.visible = true;
            this.gradientOverlay_.x = screenRect.right - 10;
            this.gradientOverlay_.y = screenRect.top;
        }
        else {
            this.gradientOverlay_.visible = false;
        }

        profObjects += getTimer() - profT2;
        // draw hw capable screen filters
        if (isGpuRender && Renderer.inGame) {
            filter = this.getFilterIndex();
            render3D = StaticInjectorContext.getInjector().getInstance(Render3D);
            render3D.dispatch(this.graphicsData_, this.graphicsData3d_, width_, height_, camera, filter);
            for (i = 0; i < this.graphicsData_.length; i++) {
                if (this.graphicsData_[i] is GraphicsBitmapFill && GraphicsFillExtra.isSoftwareDraw(GraphicsBitmapFill(this.graphicsData_[i]))) {
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i]);
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i + 1]);
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i + 2]);
                }
                else if (this.graphicsData_[i] is GraphicsSolidFill && GraphicsFillExtra.isSoftwareDrawSolid(GraphicsSolidFill(this.graphicsData_[i]))) {
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i]);
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i + 1]);
                    this.graphicsDataStageSoftware_.push(this.graphicsData_[i + 2]);
                }
            }
            if (this.graphicsDataStageSoftware_.length > 0) {
                map_.graphics.clear();
                map_.graphics.drawGraphicsData(this.graphicsDataStageSoftware_);
                if (this.lastSoftwareClear) {
                    this.lastSoftwareClear = false;
                }
            }
            else if (!this.lastSoftwareClear) {
                map_.graphics.clear();
                this.lastSoftwareClear = true;
            }
            if (time % 149 == 0) {
                GraphicsFillExtra.manageSize();
            }
        }
        else {
            map_.graphics.clear();
            map_.graphics.drawGraphicsData(this.graphicsData_);
        }

        // draw filters
        this.map_.filters.length = 0;
        if (this.player_ != null && (this.player_.condition_[ConditionEffect.CE_FIRST_BATCH] & ConditionEffect.MAP_FILTER_BITMASK) != 0) {
            var filters:Array = [];
            if (this.player_.isDrunk()) {
                d = 20 + 10 * Math.sin(time / 1000);
                filters.push(new BlurFilter(d, d));
            }
            if (this.player_.isBlind()) {
                filters.push(BLIND_FILTER);
            }
            this.map_.filters = filters;
        }
        else if (this.map_.filters.length > 0) {
            this.map_.filters = [];
        }

        this.mapOverlay_.draw(camera, time);
        this.partyOverlay_.draw(camera, time);
    }

    private function getFilterIndex():uint {
        var filterIndex:uint = 0;
        if (player_ != null && (player_.condition_[ConditionEffect.CE_FIRST_BATCH] & ConditionEffect.MAP_FILTER_BITMASK) != 0) {
            if (player_.isPaused()) {
                filterIndex = Renderer.STAGE3D_FILTER_PAUSE;
            }
            else if (player_.isBlind()) {
                filterIndex = Renderer.STAGE3D_FILTER_BLIND;
            }
            else if (player_.isDrunk()) {
                filterIndex = Renderer.STAGE3D_FILTER_DRUNK;
            }
        }
        return filterIndex;
    }
}
}
